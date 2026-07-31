package org.hyperledger.iroha.android.client;

import java.util.Objects;

/**
 * Immutable local context used to validate server-prepared transaction drafts before signing.
 *
 * <p>The chain identifier is configured by the caller and is never inferred from a server
 * response.
 */
public final class LocalSigningContext {
  private static final int MAX_CHAIN_ID_BYTES = 128;

  private final String chainId;

  public LocalSigningContext(final String chainId) {
    requireCanonicalChainId(chainId);
    this.chainId = chainId;
  }

  /** Returns the exact chain identifier required in every locally signed draft. */
  public String chainId() {
    return chainId;
  }

  @Override
  public boolean equals(final Object other) {
    return this == other
        || other instanceof LocalSigningContext
            && chainId.equals(((LocalSigningContext) other).chainId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(chainId);
  }

  private static void requireCanonicalChainId(final String value) {
    if (value == null || value.isEmpty() || value.length() > MAX_CHAIN_ID_BYTES) {
      throw new IllegalArgumentException(
          "chainId must contain 1.." + MAX_CHAIN_ID_BYTES + " ASCII bytes");
    }
    if (!isAsciiLetterOrDigit(value.charAt(0))
        || !isAsciiLetterOrDigit(value.charAt(value.length() - 1))) {
      throw new IllegalArgumentException(
          "chainId must begin and end with an ASCII alphanumeric character");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!isAsciiLetterOrDigit(character)
          && character != '.'
          && character != '_'
          && character != ':'
          && character != '-') {
        throw new IllegalArgumentException("chainId contains a non-canonical character");
      }
    }
  }

  private static boolean isAsciiLetterOrDigit(final char value) {
    return value >= 'a' && value <= 'z'
        || value >= 'A' && value <= 'Z'
        || value >= '0' && value <= '9';
  }
}
