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
import org.hyperledger.iroha.android.model.NetworkId;

/** Parsed wallet-role request from an {@code iroha://connect?...} deep link. */
public final class ConnectWalletRequest {

  private static final String SCHEME = "iroha";
  private static final String LAUNCH_SCHEME = "irohaconnect";
  private static final String HOST = "connect";
  private static final String LAUNCH_HOST = "wc";
  private static final int SID_LENGTH = 32;
  private static final byte[] SID_DOMAIN = "iroha-connect|sid|".getBytes(StandardCharsets.UTF_8);

  private final String sidBase64Url;
  private final byte[] sessionId;
  private final String token;
  private final String relayToken;
  private final NetworkId networkId;
  private final byte[] appPublicKey;
  private final byte[] nonce;
  private final URI baseUri;
  private final URI webSocketUri;

  private ConnectWalletRequest(
      final String sidBase64Url,
      final byte[] sessionId,
      final String token,
      final String relayToken,
      final NetworkId networkId,
      final byte[] appPublicKey,
      final byte[] nonce,
      final URI baseUri,
      final URI webSocketUri) {
    this.sidBase64Url = sidBase64Url;
    this.sessionId = sessionId.clone();
    this.token = token;
    this.relayToken = relayToken;
    this.networkId = Objects.requireNonNull(networkId, "networkId");
    this.appPublicKey = appPublicKey.clone();
    this.nonce = nonce.clone();
    this.baseUri = baseUri;
    this.webSocketUri = webSocketUri;
  }

  public static ConnectWalletRequest parse(final URI uri, final URI defaultBaseUri)
      throws ConnectProtocolException {
    Objects.requireNonNull(uri, "uri");
    Objects.requireNonNull(defaultBaseUri, "defaultBaseUri");
    final URI normalizedUri = normalizeConnectUri(uri);
    final String scheme = normalize(normalizedUri.getScheme());
    final String host = normalize(normalizedUri.getHost());
    if (!SCHEME.equals(scheme) || !HOST.equals(host)) {
      throw new ConnectProtocolException(
          "Connect deep link must use iroha://connect or irohaconnect://connect");
    }

    final Map<String, String> query = parseQuery(normalizedUri.getRawQuery());
    final String sid = firstPresent(query, "sid");
    if (sid == null || sid.isBlank()) {
      throw new ConnectProtocolException("Missing required query parameter: sid");
    }
    final byte[] sessionId = decodeBase64Url(sid, "sid");
    if (sessionId.length != SID_LENGTH) {
      throw new ConnectProtocolException("Connect sid must decode to 32 bytes");
    }

    final String token =
        firstNonBlank(firstPresent(query, "token_wallet"), firstPresent(query, "tokenWallet"), firstPresent(query, "token"));
    if (token == null || token.isBlank()) {
      throw new ConnectProtocolException("Missing required query parameter: token_wallet");
    }
    final String relayToken =
        firstNonBlank(firstPresent(query, "relay"), firstPresent(query, "token_relay"), firstPresent(query, "tokenRelay"));
    if (relayToken == null || relayToken.isBlank()) {
      throw new ConnectProtocolException("Missing required query parameter: relay");
    }

    if (query.containsKey("chain_id")) {
      throw new ConnectProtocolException("chain_id is retired; provide exact network_id");
    }
    final String networkIdLiteral = trimToNull(firstPresent(query, "network_id"));
    if (networkIdLiteral == null) {
      throw new ConnectProtocolException("Missing required query parameter: network_id");
    }
    final NetworkId networkId;
    try {
      networkId = NetworkId.parse(networkIdLiteral);
    } catch (final IllegalArgumentException ex) {
      throw new ConnectProtocolException("Connect network_id is not canonical", ex);
    }
    final String appPkLiteral = trimToNull(firstPresent(query, "app_pk"));
    final String nonceLiteral = trimToNull(firstPresent(query, "nonce"));
    if (appPkLiteral == null || nonceLiteral == null) {
      throw new ConnectProtocolException("Connect deep link requires app_pk and nonce");
    }
    final byte[] appPublicKey = decodeBase64Url(appPkLiteral, "app_pk");
    final byte[] nonce = decodeBase64Url(nonceLiteral, "nonce");
    if (appPublicKey.length != 32 || nonce.length != 16) {
      throw new ConnectProtocolException("Connect app_pk and nonce must decode to 32 and 16 bytes");
    }
    final byte[] sidPreimage =
        java.nio.ByteBuffer.allocate(SID_DOMAIN.length + NetworkId.BYTE_LENGTH + 32 + 16)
            .put(SID_DOMAIN)
            .put(networkId.bytes())
            .put(appPublicKey)
            .put(nonce)
            .array();
    if (!java.util.Arrays.equals(sessionId, Blake2b.digest256(sidPreimage))) {
      throw new ConnectProtocolException("Connect sid does not match network_id, app_pk, and nonce");
    }
    final URI baseUri = resolveBaseUri(trimToNull(firstPresent(query, "node")), defaultBaseUri);
    final URI wsUri = buildWalletWebSocketUri(baseUri, sid);

    return new ConnectWalletRequest(
        sid, sessionId, token, relayToken, networkId, appPublicKey, nonce, baseUri, wsUri);
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

  public String relayToken() {
    return relayToken;
  }

  public NetworkId networkId() {
    return networkId;
  }

  public byte[] appPublicKey() {
    return appPublicKey.clone();
  }

  public byte[] nonce() {
    return nonce.clone();
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

  private static URI normalizeConnectUri(final URI uri) throws ConnectProtocolException {
    final String scheme = normalize(uri.getScheme());
    final String host = normalize(uri.getHost());
    if (SCHEME.equals(scheme) && HOST.equals(host)) {
      return uri;
    }
    if (LAUNCH_SCHEME.equals(scheme) && HOST.equals(host)) {
      return rebuildCanonicalConnectUri(uri);
    }
    if (LAUNCH_SCHEME.equals(scheme) && LAUNCH_HOST.equals(host)) {
      final Map<String, String> query = parseQuery(uri.getRawQuery());
      final String embeddedUri = trimToNull(firstPresent(query, "uri"));
      if (embeddedUri == null) {
        throw new ConnectProtocolException("Missing required query parameter: uri");
      }
      try {
        return normalizeConnectUri(new URI(embeddedUri));
      } catch (final URISyntaxException ex) {
        throw new ConnectProtocolException("Embedded connect URI is malformed", ex);
      }
    }
    return uri;
  }

  private static URI rebuildCanonicalConnectUri(final URI uri)
      throws ConnectProtocolException {
    try {
      return new URI(
          SCHEME,
          uri.getRawAuthority(),
          uri.getRawPath(),
          uri.getRawQuery(),
          uri.getRawFragment());
    } catch (final URISyntaxException ex) {
      throw new ConnectProtocolException("Connect deep link URI is malformed", ex);
    }
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

  private static byte[] decodeBase64Url(final String value, final String field)
      throws ConnectProtocolException {
    try {
      String normalized = value.replace('-', '+').replace('_', '/');
      final int remainder = normalized.length() % 4;
      if (remainder != 0) {
        normalized = normalized + "=".repeat(4 - remainder);
      }
      final byte[] decoded = java.util.Base64.getDecoder().decode(normalized);
      final String canonical = java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(decoded);
      if (!canonical.equals(value)) {
        throw new ConnectProtocolException(
            "Connect " + field + " must use canonical base64url without padding");
      }
      return decoded;
    } catch (final IllegalArgumentException ex) {
      throw new ConnectProtocolException("Connect " + field + " is not valid base64url", ex);
    }
  }

  private static URI resolveBaseUri(final String nodeValue, final URI defaultUri)
      throws ConnectProtocolException {
    if (nodeValue == null || nodeValue.isEmpty()) {
      return defaultUri;
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
