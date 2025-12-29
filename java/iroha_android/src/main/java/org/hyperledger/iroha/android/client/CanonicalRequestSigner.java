package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.net.URLDecoder;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.PrivateKey;
import java.security.Signature;
import java.util.AbstractMap;
import java.util.ArrayList;
import java.util.Base64;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Builds canonical request signatures for Torii app endpoints.
 */
public final class CanonicalRequestSigner {

  public static final String HEADER_ACCOUNT = "X-Iroha-Account";
  public static final String HEADER_SIGNATURE = "X-Iroha-Signature";

  private CanonicalRequestSigner() {}

  /**
   * Canonicalise a raw query string by decoding, sorting, and re-encoding.
   */
  public static String canonicalQueryString(final String raw) {
    if (raw == null || raw.isEmpty()) {
      return "";
    }
    final List<Map.Entry<String, String>> pairs = new ArrayList<>();
    for (final String component : raw.split("&", -1)) {
      final String[] kv = component.split("=", 2);
      final String key = kv.length > 0 ? kv[0] : "";
      final String value = kv.length > 1 ? kv[1] : "";
      pairs.add(
          new AbstractMap.SimpleEntry<>(urlDecode(key), urlDecode(value)));
    }
    pairs.sort(
        (a, b) -> {
          final int keyCompare = a.getKey().compareTo(b.getKey());
          if (keyCompare != 0) {
            return keyCompare;
          }
          return a.getValue().compareTo(b.getValue());
        });
    final StringBuilder builder = new StringBuilder();
    for (int i = 0; i < pairs.size(); i++) {
      final Map.Entry<String, String> pair = pairs.get(i);
      if (i > 0) {
        builder.append('&');
      }
      builder.append(urlEncode(pair.getKey()));
      builder.append('=');
      builder.append(urlEncode(pair.getValue()));
    }
    return builder.toString();
  }

  /**
   * Build canonical request bytes for signing.
   */
  public static byte[] canonicalRequestMessage(
      final String method, final URI uri, final byte[] body) {
    final String query = canonicalQueryString(uri.getRawQuery());
    final String path = uri.getRawPath() == null ? "" : uri.getRawPath();
    final byte[] bodyBytes = body == null ? new byte[0] : body;
    final byte[] digest;
    try {
      digest = MessageDigest.getInstance("SHA-256").digest(bodyBytes);
    } catch (Exception ex) {
      throw new IllegalStateException("sha256 unavailable", ex);
    }
    final String rendered =
        method.toUpperCase(Locale.ROOT)
            + "\n"
            + path
            + "\n"
            + query
            + "\n"
            + hex(digest);
    return rendered.getBytes(StandardCharsets.UTF_8);
  }

  /**
   * Build canonical signing headers (`X-Iroha-Account`/`X-Iroha-Signature`).
   */
  public static Map<String, String> buildHeaders(
      final String method,
      final URI uri,
      final byte[] body,
      final String accountId,
      final PrivateKey privateKey) {
    if (accountId == null || accountId.isBlank()) {
      throw new IllegalArgumentException("accountId is required");
    }
    if (privateKey == null) {
      throw new IllegalArgumentException("privateKey is required");
    }
    final byte[] message = canonicalRequestMessage(method, uri, body);
    final byte[] signatureBytes;
    try {
      final Signature signer = Signature.getInstance("Ed25519");
      signer.initSign(privateKey);
      signer.update(message);
      signatureBytes = signer.sign();
    } catch (Exception ex) {
      throw new IllegalStateException("failed to sign canonical request", ex);
    }
    final Map<String, String> headers = new HashMap<>();
    headers.put(HEADER_ACCOUNT, accountId);
    headers.put(HEADER_SIGNATURE, Base64.getEncoder().encodeToString(signatureBytes));
    return headers;
  }

  private static String urlEncode(final String value) {
    try {
      return URLEncoder.encode(value, StandardCharsets.UTF_8.toString());
    } catch (Exception ex) {
      throw new IllegalStateException("failed to encode query component", ex);
    }
  }

  private static String urlDecode(final String value) {
    try {
      return URLDecoder.decode(value, StandardCharsets.UTF_8.toString());
    } catch (Exception ex) {
      throw new IllegalArgumentException("failed to decode query component", ex);
    }
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }
}
