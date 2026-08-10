package org.hyperledger.iroha.android.connect;

import java.net.URI;
import java.net.URISyntaxException;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicBoolean;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.model.NetworkId;

/** Parsed wallet-role request from an {@code iroha://connect?...} deep link. */
public final class ConnectWalletRequest {

  private static final String SCHEME = "iroha";
  private static final String HOST = "connect";
  private static final int SID_LENGTH = 32;

  private final String sidBase64Url;
  private final byte[] sessionId;
  private final String token;
  private final String relayToken;
  private final NetworkId networkId;
  private final byte[] appPublicKey;
  private final byte[] nonce;
  private final URI baseUri;
  private final URI webSocketUri;
  private final AtomicBoolean openAccepted = new AtomicBoolean(false);

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
    if (!SCHEME.equals(uri.getScheme())
        || !HOST.equals(uri.getHost())
        || (uri.getRawPath() != null && !uri.getRawPath().isEmpty())
        || uri.getRawFragment() != null
        || uri.getRawUserInfo() != null) {
      throw new ConnectProtocolException("Connect deep link must use canonical iroha://connect");
    }

    final Map<String, String> query = parseQuery(uri.getRawQuery());
    final String sid = query.get("sid");
    if (sid == null || sid.isEmpty()) {
      throw new ConnectProtocolException("Missing required query parameter: sid");
    }
    final byte[] sessionId = decodeBase64Url(sid, "sid");
    if (sessionId.length != SID_LENGTH) {
      throw new ConnectProtocolException("Connect sid must decode to 32 bytes");
    }

    for (final String retired :
        new String[] {"chain_id", "token_wallet", "tokenWallet", "token_relay", "tokenRelay"}) {
      if (query.containsKey(retired)) {
        throw new ConnectProtocolException("Retired Connect query parameter: " + retired);
      }
    }
    final String token = query.get("token");
    if (token == null || token.isBlank() || !token.trim().equals(token)) {
      throw new ConnectProtocolException("Missing or invalid required query parameter: token");
    }
    final String relayToken = query.get("relay");
    if (relayToken == null || relayToken.isBlank() || !relayToken.trim().equals(relayToken)) {
      throw new ConnectProtocolException("Missing required query parameter: relay");
    }
    if (!"1".equals(query.get("v"))) {
      throw new ConnectProtocolException("Connect v must be exactly 1");
    }
    if (!"wallet".equals(query.get("role"))) {
      throw new ConnectProtocolException("Connect role must be exactly wallet");
    }
    final String networkIdLiteral = query.get("network_id");
    if (networkIdLiteral == null) {
      throw new ConnectProtocolException("Missing required query parameter: network_id");
    }
    final NetworkId networkId;
    try {
      networkId = NetworkId.parse(networkIdLiteral);
    } catch (final IllegalArgumentException ex) {
      throw new ConnectProtocolException("Connect network_id is not canonical", ex);
    }
    final String appPkLiteral = query.get("app_pk");
    final String nonceLiteral = query.get("nonce");
    if (appPkLiteral == null || nonceLiteral == null) {
      throw new ConnectProtocolException("Connect deep link requires app_pk and nonce");
    }
    final byte[] appPublicKey = decodeBase64Url(appPkLiteral, "app_pk");
    final byte[] nonce = decodeBase64Url(nonceLiteral, "nonce");
    if (appPublicKey.length != 32 || nonce.length != 16) {
      throw new ConnectProtocolException("Connect app_pk and nonce must decode to 32 and 16 bytes");
    }
    if (!java.util.Arrays.equals(
        sessionId, ConnectCrypto.deriveSessionId(networkId, appPublicKey, nonce))) {
      throw new ConnectProtocolException("Connect sid does not match network_id, app_pk, and nonce");
    }
    final URI baseUri = resolveBaseUri(query.get("node"), defaultBaseUri);
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

  /** Validates and consumes the one permitted application {@code Open} frame. */
  public ConnectFrameCodec.OpenControl acceptOpen(final byte[] rawFrame)
      throws ConnectProtocolException {
    final ConnectFrameCodec.DecodedFrame frame = ConnectFrameCodec.decode(rawFrame);
    if (!java.util.Arrays.equals(frame.sessionId(), sessionId)) {
      throw new ConnectProtocolException("Connect Open sid does not match the launch request");
    }
    if (frame.direction() != ConnectDirection.APP_TO_WALLET || frame.sequence() != 1L) {
      throw new ConnectProtocolException("Connect Open must be app-to-wallet sequence 1");
    }
    final ConnectFrameCodec.OpenControl open = frame.open();
    if (open == null) {
      throw new ConnectProtocolException("Expected a Connect Open control frame");
    }
    if (!java.util.Arrays.equals(open.appPublicKey(), appPublicKey)) {
      throw new ConnectProtocolException("Connect Open app_pk does not match the launch request");
    }
    if (!networkId.equals(open.networkId())) {
      throw new ConnectProtocolException("Connect Open network_id does not match the launch request");
    }
    if (!openAccepted.compareAndSet(false, true)) {
      throw new ConnectProtocolException("Connect Open was already accepted");
    }
    return open;
  }

  /** Builds the exact approval preimage after the launch-bound {@code Open} has been consumed. */
  public byte[] buildApprovePreimage(
      final byte[] walletPublicKey,
      final String accountId,
      final byte[] permissionsHash,
      final byte[] proofHash)
      throws ConnectProtocolException {
    if (!openAccepted.get()) {
      throw new ConnectProtocolException("Connect Open must be accepted before approval");
    }
    return ConnectCrypto.buildApprovePreimage(
        networkId,
        sessionId,
        appPublicKey,
        walletPublicKey,
        accountId,
        permissionsHash,
        proofHash,
        ConnectCrypto.relayAuthHash(sessionId, relayToken));
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

  private static Map<String, String> parseQuery(final String rawQuery)
      throws ConnectProtocolException {
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
      final String key;
      final String value;
      try {
        key = urlDecode(rawKey);
        value = urlDecode(rawValue);
      } catch (final IllegalArgumentException ex) {
        throw new ConnectProtocolException("Connect query contains invalid percent encoding", ex);
      }
      if (query.put(key, value) != null) {
        throw new ConnectProtocolException("Duplicate Connect query parameter: " + key);
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
