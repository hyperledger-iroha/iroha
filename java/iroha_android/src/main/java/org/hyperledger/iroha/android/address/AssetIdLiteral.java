package org.hyperledger.iroha.android.address;

import java.util.Locale;
import java.util.Objects;

/** Helpers for normalizing encoded asset identifier literals. */
public final class AssetIdLiteral {

  private static final String PREFIX = "norito:";
  private static final boolean NATIVE_AVAILABLE;

  static {
    boolean available;
    try {
      System.loadLibrary("connect_norito_bridge");
      available = true;
    } catch (UnsatisfiedLinkError error) {
      available = false;
    }
    NATIVE_AVAILABLE = available;
  }

  private AssetIdLiteral() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

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

  /**
   * Builds a canonical encoded asset identifier from textual parts.
   *
   * <p>This method accepts only canonical unprefixed Base58 asset-definition addresses.
   * Alias-shaped definitions are rejected because alias resolution requires online access.
   *
   * @param assetDefinitionId asset definition literal
   * @param accountId canonical account identifier (I105)
   * @return canonical encoded asset identifier (`norito:<hex>`)
   */
  public static String encodeFromParts(
      final String assetDefinitionId, final String accountId) {
    final String normalizedDefinition = normalizeAssetDefinitionLiteral(assetDefinitionId);
    final String normalizedAccount =
        AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("connect_norito_bridge is not available in this runtime");
    }
    final String encoded = nativeEncodeFromParts(normalizedDefinition, normalizedAccount);
    if (encoded == null) {
      throw new IllegalStateException("nativeEncodeFromParts returned null");
    }
    return normalizeEncoded(encoded, "assetId");
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

  private static String normalizeAssetDefinitionLiteral(final String assetDefinitionId) {
    final String trimmed = Objects.requireNonNull(assetDefinitionId, "assetDefinitionId").trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException("assetDefinitionId must not be blank");
    }
    if (trimmed.indexOf('@') >= 0 || trimmed.indexOf('#') >= 0) {
      throw new IllegalArgumentException(
          "assetDefinitionId aliases or text selectors require online resolution; provide canonical definition id");
    }
    if (trimmed.indexOf(' ') >= 0 || trimmed.indexOf('\t') >= 0 || trimmed.indexOf('\n') >= 0) {
      throw new IllegalArgumentException("assetDefinitionId must not contain whitespace");
    }
    if (!AssetDefinitionIdEncoder.isCanonicalAddress(trimmed)) {
      throw new IllegalArgumentException(
          "assetDefinitionId must use canonical unprefixed Base58 form");
    }
    return trimmed;
  }

  private static native String nativeEncodeFromParts(String assetDefinitionId, String accountId);
}
