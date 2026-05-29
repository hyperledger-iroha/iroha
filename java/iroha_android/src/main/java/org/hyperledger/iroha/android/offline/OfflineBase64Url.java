package org.hyperledger.iroha.android.offline;

import java.util.Base64;
import java.util.Objects;

/** Strict base64url helpers for Offline Note text payloads. */
final class OfflineBase64Url {
  private OfflineBase64Url() {}

  static byte[] decodeUnpadded(final String value, final String field) {
    final String payload = Objects.requireNonNull(value, field);
    if (payload.trim().isEmpty() || payload.indexOf('=') >= 0) {
      throw new IllegalArgumentException(field + " must be unpadded base64url");
    }
    for (int index = 0; index < payload.length(); index++) {
      final char ch = payload.charAt(index);
      if (!isBase64UrlCharacter(ch)) {
        throw new IllegalArgumentException(field + " must be unpadded base64url");
      }
    }
    return Base64.getUrlDecoder().decode(payload);
  }

  private static boolean isBase64UrlCharacter(final char ch) {
    return (ch >= 'A' && ch <= 'Z')
        || (ch >= 'a' && ch <= 'z')
        || (ch >= '0' && ch <= '9')
        || ch == '-'
        || ch == '_';
  }
}
