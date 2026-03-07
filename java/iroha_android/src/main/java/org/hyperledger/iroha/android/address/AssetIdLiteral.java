package org.hyperledger.iroha.android.address;

import java.util.Locale;
import java.util.Objects;

/** Helpers for normalizing encoded asset identifier literals. */
public final class AssetIdLiteral {

  private static final String PREFIX = "norito:";

  private AssetIdLiteral() {}

  /**
   * Normalizes an encoded asset identifier.
   *
   * <p>Accepted format is {@code norito:<hex>}.
   *
   * @param assetId asset identifier string
   * @return canonical encoded asset identifier with a lowercase hex payload
   */
  public static String normalizeEncoded(final String assetId) {
    return normalizeEncoded(assetId, "assetId");
  }

  /**
   * Normalizes an encoded asset identifier with a custom field label.
   *
   * @param assetId asset identifier string
   * @param field field name used in validation messages
   * @return canonical encoded asset identifier with a lowercase hex payload
   */
  public static String normalizeEncoded(final String assetId, final String field) {
    Objects.requireNonNull(field, "field");
    final String trimmed = Objects.requireNonNull(assetId, field).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    if (!trimmed.regionMatches(true, 0, PREFIX, 0, PREFIX.length())) {
      throw new IllegalArgumentException(field + " must be encoded as norito:<hex>");
    }
    final String hex = trimmed.substring(PREFIX.length());
    if (hex.isEmpty() || (hex.length() & 1) != 0 || !isHex(hex)) {
      throw new IllegalArgumentException(field + " must be encoded as norito:<hex>");
    }
    return PREFIX + hex.toLowerCase(Locale.ROOT);
  }

  private static boolean isHex(final String value) {
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      final boolean digit = ch >= '0' && ch <= '9';
      final boolean lower = ch >= 'a' && ch <= 'f';
      final boolean upper = ch >= 'A' && ch <= 'F';
      if (!digit && !lower && !upper) {
        return false;
      }
    }
    return true;
  }
}
