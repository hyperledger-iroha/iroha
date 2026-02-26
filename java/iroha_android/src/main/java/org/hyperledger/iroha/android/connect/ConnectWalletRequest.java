package org.hyperledger.iroha.android.connect;

import java.net.URI;
import java.net.URISyntaxException;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** Parsed wallet-role request from an {@code iroha://connect?...} deep link. */
public final class ConnectWalletRequest {

  private static final String SCHEME = "iroha";
  private static final String HOST = "connect";
  private static final int SID_LENGTH = 32;

  private final String sidBase64Url;
  private final byte[] sessionId;
  private final String token;
  private final String chainId;
  private final URI baseUri;
  private final URI webSocketUri;

  private ConnectWalletRequest(
      final String sidBase64Url,
      final byte[] sessionId,
      final String token,
      final String chainId,
      final URI baseUri,
      final URI webSocketUri) {
    this.sidBase64Url = sidBase64Url;
    this.sessionId = sessionId.clone();
    this.token = token;
    this.chainId = chainId;
    this.baseUri = baseUri;
    this.webSocketUri = webSocketUri;
  }

  public static ConnectWalletRequest parse(final URI uri, final URI defaultBaseUri)
      throws ConnectProtocolException {
    Objects.requireNonNull(uri, "uri");
    Objects.requireNonNull(defaultBaseUri, "defaultBaseUri");
    final String scheme = normalize(uri.getScheme());
    final String host = normalize(uri.getHost());
    if (!SCHEME.equals(scheme) || !HOST.equals(host)) {
      throw new ConnectProtocolException("Connect deep link must use iroha://connect");
    }

    final Map<String, String> query = parseQuery(uri.getRawQuery());
    final String sid = firstPresent(query, "sid");
    if (sid == null || sid.isBlank()) {
      throw new ConnectProtocolException("Missing required query parameter: sid");
    }
    final byte[] sessionId = decodeBase64Url(sid);
    if (sessionId.length != SID_LENGTH) {
      throw new ConnectProtocolException("Connect sid must decode to 32 bytes");
    }

    final String token =
        firstNonBlank(firstPresent(query, "token_wallet"), firstPresent(query, "tokenWallet"), firstPresent(query, "token"));
    if (token == null || token.isBlank()) {
      throw new ConnectProtocolException("Missing required query parameter: token_wallet");
    }

    final String chainId = trimToNull(firstPresent(query, "chain_id"));
    final URI baseUri = resolveBaseUri(trimToNull(firstPresent(query, "node")), defaultBaseUri);
    final URI wsUri = buildWalletWebSocketUri(baseUri, sid);

    return new ConnectWalletRequest(sid, sessionId, token, chainId, baseUri, wsUri);
  }

  public static ConnectWalletRequest parse(final String rawUri, final URI defaultBaseUri)
      throws ConnectProtocolException {
    Objects.requireNonNull(rawUri, "rawUri");
    try {
      return parse(new URI(rawUri), defaultBaseUri);
    } catch (final URISyntaxException ex) {
      throw new ConnectProtocolException("Connect deep link URI is malformed", ex);
    }
  }

  public String sidBase64Url() {
    return sidBase64Url;
  }

  public byte[] sessionId() {
    return sessionId.clone();
  }

  public String token() {
    return token;
  }

  public String chainId() {
    return chainId;
  }

  public URI baseUri() {
    return baseUri;
  }

  public URI webSocketUri() {
    return webSocketUri;
  }

  /**
   * Stable short fingerprint used by UI/testing to correlate sessions without exposing full tokens.
   */
  public String sessionFingerprintHex() {
    final byte[] digest = Blake2b.digest(sessionId, 8);
    final StringBuilder builder = new StringBuilder(digest.length * 2);
    for (final byte b : digest) {
      builder.append(String.format(Locale.ROOT, "%02x", b & 0xFF));
    }
    return builder.toString();
  }

  private static String normalize(final String value) {
    if (value == null) {
      return "";
    }
    return value.trim().toLowerCase(Locale.ROOT);
  }

  private static String trimToNull(final String value) {
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  private static String firstPresent(final Map<String, String> map, final String key) {
    return map.get(key);
  }

  private static String firstNonBlank(final String... values) {
    for (final String value : values) {
      if (value != null && !value.trim().isEmpty()) {
        return value.trim();
      }
    }
    return null;
  }

  private static Map<String, String> parseQuery(final String rawQuery) {
    final Map<String, String> query = new LinkedHashMap<>();
    if (rawQuery == null || rawQuery.isEmpty()) {
      return query;
    }
    for (final String part : rawQuery.split("&")) {
      if (part == null || part.isEmpty()) {
        continue;
      }
      final int idx = part.indexOf('=');
      final String rawKey = idx >= 0 ? part.substring(0, idx) : part;
      final String rawValue = idx >= 0 ? part.substring(idx + 1) : "";
      final String key = urlDecode(rawKey);
      if (!query.containsKey(key)) {
        query.put(key, urlDecode(rawValue));
      }
    }
    return query;
  }

  private static String urlDecode(final String value) {
    return URLDecoder.decode(value, StandardCharsets.UTF_8);
  }

  private static byte[] decodeBase64Url(final String value) throws ConnectProtocolException {
    try {
      String normalized = value.replace('-', '+').replace('_', '/');
      final int remainder = normalized.length() % 4;
      if (remainder != 0) {
        normalized = normalized + "=".repeat(4 - remainder);
      }
      return java.util.Base64.getDecoder().decode(normalized);
    } catch (final IllegalArgumentException ex) {
      throw new ConnectProtocolException("Connect sid is not valid base64url", ex);
    }
  }

  private static URI resolveBaseUri(final String nodeValue, final URI fallback)
      throws ConnectProtocolException {
    if (nodeValue == null || nodeValue.isEmpty()) {
      return fallback;
    }
    URI parsed = tryParse(nodeValue);
    if (parsed != null && parsed.getScheme() != null && parsed.getHost() != null) {
      final String normalizedScheme = normalize(parsed.getScheme());
      if ("http".equals(normalizedScheme) || "https".equals(normalizedScheme)) {
        return parsed;
      }
    }

    parsed = tryParse("https://" + nodeValue);
    if (parsed != null && parsed.getHost() != null) {
      return parsed;
    }

    throw new ConnectProtocolException("Invalid node parameter in connect link: " + nodeValue);
  }

  private static URI tryParse(final String raw) {
    try {
      return new URI(raw);
    } catch (final URISyntaxException ignored) {
      return null;
    }
  }

  private static URI buildWalletWebSocketUri(final URI base, final String sid)
      throws ConnectProtocolException {
    final String scheme = normalize(base.getScheme());
    final String wsScheme;
    if ("https".equals(scheme)) {
      wsScheme = "wss";
    } else if ("http".equals(scheme)) {
      wsScheme = "ws";
    } else {
      throw new ConnectProtocolException("Connect base URI must use http/https");
    }

    final String host = base.getHost();
    if (host == null || host.isBlank()) {
      throw new ConnectProtocolException("Connect base URI is missing host");
    }

    final int port = base.getPort();
    final String query = "sid=" + sid + "&role=wallet";
    try {
      return new URI(wsScheme, null, host, port, "/v1/connect/ws", query, null);
    } catch (final URISyntaxException ex) {
      throw new ConnectProtocolException("Failed to build connect websocket URI", ex);
    }
  }
}
