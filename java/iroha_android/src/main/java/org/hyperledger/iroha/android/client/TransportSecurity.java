package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.Map;

/**
 * Shared transport-safety checks for SDK requests that carry credentials or raw private keys.
 */
public final class TransportSecurity {

  private TransportSecurity() {}

  /** Returns {@code true} when the headers carry credential material. */
  public static boolean headersContainCredentials(final Map<String, String> headers) {
    if (headers == null || headers.isEmpty()) {
      return false;
    }
    for (final String key : headers.keySet()) {
      if (key == null) {
        continue;
      }
      final String normalized = key.toLowerCase(Locale.ROOT);
      if ("authorization".equals(normalized) || "x-api-token".equals(normalized)) {
        return true;
      }
    }
    return false;
  }

  /** Rejects insecure or mismatched HTTP requests that carry credentials or raw private keys. */
  public static void requireHttpRequestAllowed(
      final String context,
      final URI baseUri,
      final URI targetUri,
      final Map<String, String> headers,
      final byte[] body) {
    if (!isSensitive(headers, body)) {
      return;
    }
    final String targetScheme = normalize(targetUri.getScheme());
    if (!"https".equals(targetScheme)) {
      throw new IllegalArgumentException(
          context + " refuses insecure transport over " + renderScheme(targetScheme) + "; use https.");
    }
    final String baseScheme = normalize(baseUri.getScheme());
    if (!targetScheme.equals(baseScheme)) {
      throw new IllegalArgumentException(
          context
              + " refuses sensitive requests over mismatched scheme "
              + renderScheme(targetScheme)
              + "; use relative paths derived from the configured base URL.");
    }
    if (!sameAuthority(baseUri, targetUri, "https")) {
      throw new IllegalArgumentException(
          context
              + " refuses sensitive requests to mismatched host "
              + renderHost(targetUri)
              + "; use relative paths on the configured base URL.");
    }
  }

  /** Rejects insecure or mismatched WebSocket requests that carry credentials. */
  public static void requireWebSocketRequestAllowed(
      final String context,
      final URI baseUri,
      final URI targetUri,
      final Map<String, String> headers) {
    if (!headersContainCredentials(headers)) {
      return;
    }
    final String expectedScheme = expectedWebSocketScheme(baseUri);
    final String targetScheme = normalize(targetUri.getScheme());
    if (!targetScheme.equals(expectedScheme)) {
      throw new IllegalArgumentException(
          context
              + " refuses credentialed WebSocket requests over mismatched scheme "
              + renderScheme(targetScheme)
              + "; use "
              + expectedScheme
              + " URLs derived from the configured base URL.");
    }
    if (!sameAuthority(baseUri, targetUri, expectedScheme)) {
      throw new IllegalArgumentException(
          context
              + " refuses credentialed WebSocket requests to mismatched host "
              + renderHost(targetUri)
              + "; use the configured base URL host.");
    }
    if (!"wss".equals(targetScheme)) {
      throw new IllegalArgumentException(
          context
              + " refuses insecure WebSocket protocol "
              + renderScheme(targetScheme)
              + "; use wss.");
    }
  }

  private static boolean isSensitive(final Map<String, String> headers, final byte[] body) {
    return headersContainCredentials(headers) || bodyContainsPrivateKey(body);
  }

  private static boolean bodyContainsPrivateKey(final byte[] body) {
    if (body == null || body.length == 0) {
      return false;
    }
    final String rendered = new String(body, StandardCharsets.UTF_8).toLowerCase(Locale.ROOT);
    return rendered.contains("\"private_key\"");
  }

  private static String expectedWebSocketScheme(final URI baseUri) {
    final String scheme = normalize(baseUri.getScheme());
    if ("https".equals(scheme) || "wss".equals(scheme)) {
      return "wss";
    }
    return "ws";
  }

  private static boolean sameAuthority(
      final URI lhs, final URI rhs, final String defaultPortScheme) {
    final String lhsHost = normalize(lhs.getHost());
    final String rhsHost = normalize(rhs.getHost());
    return lhsHost.equals(rhsHost)
        && effectivePort(lhs, defaultPortScheme) == effectivePort(rhs, defaultPortScheme);
  }

  private static int effectivePort(final URI uri, final String defaultPortScheme) {
    if (uri.getPort() != -1) {
      return uri.getPort();
    }
    final String scheme =
        normalize(uri.getScheme()).isEmpty() ? defaultPortScheme : normalize(uri.getScheme());
    if ("http".equals(scheme) || "ws".equals(scheme)) {
      return 80;
    }
    if ("https".equals(scheme) || "wss".equals(scheme)) {
      return 443;
    }
    return -1;
  }

  private static String renderHost(final URI uri) {
    return uri.getHost() == null ? uri.toString() : uri.getHost();
  }

  private static String renderScheme(final String scheme) {
    return scheme == null || scheme.isEmpty() ? "unknown" : scheme;
  }

  private static String normalize(final String value) {
    return value == null ? "" : value.toLowerCase(Locale.ROOT);
  }
}
