package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** A reason Torii cannot currently accept Offline operations for an asset definition. */
public final class OfflineReadinessBlocker {
  private final String code;
  private final String message;

  public OfflineReadinessBlocker(final String code, final String message) {
    this.code = requireExactText(code, "code");
    this.message = requireExactText(message, "message");
  }

  public String code() {
    return code;
  }

  public String message() {
    return message;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof OfflineReadinessBlocker)) {
      return false;
    }
    final OfflineReadinessBlocker that = (OfflineReadinessBlocker) other;
    return code.equals(that.code) && message.equals(that.message);
  }

  @Override
  public int hashCode() {
    return Objects.hash(code, message);
  }

  private static String requireExactText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }
}
