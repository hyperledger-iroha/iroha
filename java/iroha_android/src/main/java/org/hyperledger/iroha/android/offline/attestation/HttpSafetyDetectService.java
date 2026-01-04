package org.hyperledger.iroha.android.offline.attestation;

import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.time.Clock;
import java.time.Duration;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.PlatformHttpTransportExecutor;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/**
 * Default {@link SafetyDetectService} backed by HTTP endpoints provided by Huawei. Tokens are cached
 * locally so the SDK does not fetch OAuth credentials for every attestation attempt.
 */
public final class HttpSafetyDetectService implements SafetyDetectService {

  private final HttpTransportExecutor executor;
  private final SafetyDetectOptions options;
  private final Clock clock;
  private final AtomicReference<AccessToken> cachedToken = new AtomicReference<>();

  public HttpSafetyDetectService(
      final HttpTransportExecutor executor, final SafetyDetectOptions options) {
    this(executor, options, Clock.systemUTC());
  }

  /**
   * Creates a service backed by the platform HTTP executor (OkHttp on Android, JDK client on JVM).
   */
  public static HttpSafetyDetectService createDefault(final SafetyDetectOptions options) {
    return new HttpSafetyDetectService(PlatformHttpTransportExecutor.createDefault(), options);
  }

  HttpSafetyDetectService(
      final HttpTransportExecutor executor,
      final SafetyDetectOptions options,
      final Clock clock) {
    this.executor = Objects.requireNonNull(executor, "executor");
    this.options = Objects.requireNonNull(options, "options");
    this.clock = Objects.requireNonNull(clock, "clock");
    if (!options.enabled()) {
      throw new IllegalArgumentException("SafetyDetectOptions must be enabled");
    }
  }

  @Override
  public CompletableFuture<SafetyDetectAttestation> fetch(
      final SafetyDetectRequest request) {
    Objects.requireNonNull(request, "request");
    return resolveAccessToken()
        .thenCompose(token -> executeAttestation(request, token));
  }

  private CompletableFuture<SafetyDetectAttestation> executeAttestation(
      final SafetyDetectRequest request, final String accessToken) {
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("app_id", request.appId());
    payload.put("nonce", encodeNonce(request.nonceHex()));
    payload.put("package_name", request.packageName());
    payload.put("sign_cert_sha256", request.signingDigestSha256());
    payload.put("certificate_id_hex", request.certificateIdHex());
    final List<String> evaluations = request.requiredEvaluations();
    if (evaluations != null && !evaluations.isEmpty()) {
      payload.put("required_evaluations", evaluations);
    }
    final Long maxAge = request.maxTokenAgeMs();
    if (maxAge != null && maxAge > 0) {
      payload.put("max_token_age_ms", maxAge);
    }
    final String body = JsonEncoder.encode(payload);
    final TransportRequest httpRequest =
        TransportRequest.builder()
            .setUri(options.attestationEndpoint())
            .setMethod("POST")
            .setTimeout(options.requestTimeout())
            .addHeader("Content-Type", "application/json")
            .addHeader("Authorization", "Bearer " + accessToken)
            .setBody(body.getBytes(StandardCharsets.UTF_8))
            .build();
    final long fetchedAt = clock.millis();
    return executor
        .execute(httpRequest)
        .thenApply(response -> parseAttestation(response, fetchedAt));
  }

  private SafetyDetectAttestation parseAttestation(
      final TransportResponse response, final long fetchedAt) {
    if (response.statusCode() != 200) {
      throw new SafetyDetectException(
          "Safety Detect attestation failed with status "
              + response.statusCode()
              + ": "
              + response.message());
    }
    final Object parsed =
        JsonParser.parse(new String(response.body(), StandardCharsets.UTF_8));
    if (!(parsed instanceof Map<?, ?> map)) {
      throw new SafetyDetectException("Safety Detect attestation response is not a JSON object");
    }
    final Object tokenValue = map.get("token");
    if (!(tokenValue instanceof String token) || token.isBlank()) {
      throw new SafetyDetectException("Safety Detect attestation response missing token");
    }
    return new SafetyDetectAttestation(token, fetchedAt);
  }

  private CompletableFuture<String> resolveAccessToken() {
    final AccessToken cached = cachedToken.get();
    final long now = clock.millis();
    if (cached != null && cached.isValid(now)) {
      return CompletableFuture.completedFuture(cached.value);
    }
    final TransportRequest request =
        TransportRequest.builder()
            .setUri(options.oauthEndpoint())
            .setMethod("POST")
            .setTimeout(options.requestTimeout())
            .addHeader("Content-Type", "application/x-www-form-urlencoded")
            .setBody(buildOauthBody().getBytes(StandardCharsets.UTF_8))
            .build();
    return executor
        .execute(request)
        .thenApply(response -> parseAccessToken(response, now));
  }

  private String buildOauthBody() {
    final StringBuilder builder = new StringBuilder();
    builder.append("grant_type=client_credentials");
    builder.append("&client_id=").append(urlEncode(options.clientId()));
    builder.append("&client_secret=").append(urlEncode(options.clientSecret()));
    return builder.toString();
  }

  private String parseAccessToken(final TransportResponse response, final long requestedAtMs) {
    if (response.statusCode() != 200) {
      throw new SafetyDetectException(
          "Safety Detect OAuth failed with status "
              + response.statusCode()
              + ": "
              + response.message());
    }
    final Object parsed =
        JsonParser.parse(new String(response.body(), StandardCharsets.UTF_8));
    if (!(parsed instanceof Map<?, ?> map)) {
      throw new SafetyDetectException("Safety Detect OAuth response is not a JSON object");
    }
    final Object tokenValue = map.get("access_token");
    if (!(tokenValue instanceof String token) || token.isBlank()) {
      throw new SafetyDetectException("Safety Detect OAuth response missing access_token");
    }
    final long expirySeconds = asLong(map.get("expires_in"), "expires_in");
    final long expiresAt =
        requestedAtMs
            + Duration.ofSeconds(expirySeconds).toMillis()
            - options.tokenSkew().toMillis();
    final AccessToken newToken =
        new AccessToken(token, Math.max(expiresAt, requestedAtMs));
    cachedToken.set(newToken);
    return newToken.value;
  }

  private static long asLong(final Object value, final String field) {
    if (value instanceof Number number) {
      return number.longValue();
    }
    if (value == null) {
      throw new SafetyDetectException("Safety Detect OAuth missing " + field);
    }
    try {
      return Long.parseLong(String.valueOf(value));
    } catch (final NumberFormatException ex) {
      throw new SafetyDetectException("Safety Detect OAuth field " + field + " is not numeric", ex);
    }
  }

  private static String encodeNonce(final String hex) {
    final byte[] decoded = decodeHex(hex);
    return Base64.getEncoder().encodeToString(decoded);
  }

  private static byte[] decodeHex(final String hex) {
    if (hex == null || hex.isBlank()) {
      return new byte[0];
    }
    final String normalized = hex.trim();
    if (normalized.length() % 2 != 0) {
      throw new SafetyDetectException("Nonce hex must have an even length");
    }
    final byte[] out = new byte[normalized.length() / 2];
    for (int i = 0; i < normalized.length(); i += 2) {
      final String slice = normalized.substring(i, i + 2);
      out[i / 2] = (byte) Integer.parseInt(slice, 16);
    }
    return out;
  }

  private static String urlEncode(final String value) {
    return URLEncoder.encode(value, StandardCharsets.UTF_8);
  }

  private static final class AccessToken {
    private final String value;
    private final long expiresAtMs;

    private AccessToken(final String value, final long expiresAtMs) {
      this.value = value;
      this.expiresAtMs = expiresAtMs;
    }

    boolean isValid(final long nowMs) {
      return nowMs < expiresAtMs;
    }
  }
}
