package org.hyperledger.iroha.android.client;

import java.util.Arrays;
import java.util.Base64;
import java.util.Objects;

/**
 * Opaque issuer credential for the first-release Bootle/Lantern issuance routes.
 *
 * <p>The credential is retained as private bytes, rendered as canonical unpadded base64url only
 * while constructing a request, never exposed by {@link #toString()}, and erased when {@link
 * #close()} is called.
 */
public final class BootleLanternIssuanceCredentialV1 implements AutoCloseable {
  /** Maximum decoded credential length accepted by Torii. */
  public static final int MAX_BYTES = 4_096;

  private static final int MAX_ENCODED_BYTES = ((MAX_BYTES + 2) / 3) * 4;

  private final byte[] credentialBytes;
  private boolean closed;

  private BootleLanternIssuanceCredentialV1(final byte[] credential) {
    this.credentialBytes = credential.clone();
  }

  /** Copies and validates opaque credential bytes. */
  public static BootleLanternIssuanceCredentialV1 fromOpaqueBytes(final byte[] credential) {
    Objects.requireNonNull(credential, "credential");
    if (credential.length == 0) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance credential must not be empty");
    }
    if (credential.length > MAX_BYTES) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance credential exceeds " + MAX_BYTES + " bytes");
    }
    return new BootleLanternIssuanceCredentialV1(credential);
  }

  /**
   * Decodes exactly one canonical, unpadded base64url credential without a {@code Bearer} prefix.
   */
  public static BootleLanternIssuanceCredentialV1 fromCanonicalBase64Url(
      final String encoded) {
    Objects.requireNonNull(encoded, "encoded");
    if (encoded.isEmpty()) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance credential must not be empty");
    }
    if (encoded.length() > MAX_ENCODED_BYTES) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance credential encoding is too long");
    }
    for (int index = 0; index < encoded.length(); index++) {
      final char character = encoded.charAt(index);
      if (character == '=' || Character.isWhitespace(character)) {
        throw new IllegalArgumentException(
            "Bootle/Lantern issuance credential must be canonical unpadded base64url");
      }
    }
    final byte[] decoded;
    try {
      decoded = Base64.getUrlDecoder().decode(encoded);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance credential must be canonical unpadded base64url", error);
    }
    try {
      if (decoded.length == 0 || decoded.length > MAX_BYTES) {
        throw new IllegalArgumentException(
            "Bootle/Lantern issuance credential must contain 1.." + MAX_BYTES + " bytes");
      }
      if (!Base64.getUrlEncoder().withoutPadding().encodeToString(decoded).equals(encoded)) {
        throw new IllegalArgumentException(
            "Bootle/Lantern issuance credential must be canonical unpadded base64url");
      }
      return new BootleLanternIssuanceCredentialV1(decoded);
    } finally {
      Arrays.fill(decoded, (byte) 0);
    }
  }

  synchronized String authorizationHeaderValue() {
    if (closed) {
      throw new IllegalStateException("Bootle/Lantern issuance credential is closed");
    }
    return "Bearer "
        + Base64.getUrlEncoder().withoutPadding().encodeToString(credentialBytes);
  }

  /** Erases the retained credential bytes. */
  @Override
  public synchronized void close() {
    if (!closed) {
      Arrays.fill(credentialBytes, (byte) 0);
      closed = true;
    }
  }

  /** Returns a redacted diagnostic representation. */
  @Override
  public String toString() {
    return "BootleLanternIssuanceCredentialV1([REDACTED])";
  }
}
