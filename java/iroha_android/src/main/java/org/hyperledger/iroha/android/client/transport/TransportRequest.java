package org.hyperledger.iroha.android.client.transport;

import java.net.URI;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/** SDK-owned transport request wrapper to decouple callers from {@code java.net.http.HttpRequest}. */
public final class TransportRequest {

  private final String method;
  private final URI uri;
  private final Map<String, List<String>> headers;
  private final byte[] body;
  private final Duration timeout;
  private final Long maximumResponseBytes;
  private final RequestReplayPolicy replayPolicy;

  private TransportRequest(
      final String method,
      final URI uri,
      final Map<String, List<String>> headers,
      final byte[] body,
      final Duration timeout,
      final Long maximumResponseBytes) {
    this.method = Objects.requireNonNull(method, "method");
    this.uri = Objects.requireNonNull(uri, "uri");
    this.headers = Collections.unmodifiableMap(headers);
    this.body = body == null ? new byte[0] : body.clone();
    this.timeout = timeout;
    if (maximumResponseBytes != null) {
      BoundedResponseBodyReader.validateMaximum(maximumResponseBytes.longValue());
    }
    this.maximumResponseBytes = maximumResponseBytes;
    this.replayPolicy = deriveReplayPolicy(method, headers, this.body);
  }

  public String method() {
    return method;
  }

  public URI uri() {
    return uri;
  }

  public Map<String, List<String>> headers() {
    return headers;
  }

  public byte[] body() {
    return body.clone();
  }

  /** Optional per-request timeout. A {@code null} value indicates executor defaults should apply. */
  public Duration timeout() {
    return timeout;
  }

  /** Optional inclusive buffered response-body limit, or {@code null} for the executor limit. */
  public Long maximumResponseBytes() {
    return maximumResponseBytes;
  }

  /** Replay policy derived from the immutable request method, headers, and body. */
  public RequestReplayPolicy replayPolicy() {
    return replayPolicy;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {

    private String method = "GET";
    private URI uri = URI.create("http://localhost/");
    private final Map<String, List<String>> headers = new java.util.LinkedHashMap<>();
    private byte[] body = new byte[0];
    private Duration timeout = null;
    private Long maximumResponseBytes = null;

    public Builder setMethod(final String method) {
      this.method = Objects.requireNonNull(method, "method");
      return this;
    }

    public Builder setUri(final URI uri) {
      this.uri = Objects.requireNonNull(uri, "uri");
      return this;
    }

    public Builder addHeader(final String name, final String value) {
      headers.computeIfAbsent(Objects.requireNonNull(name, "name"), ignored -> new ArrayList<>())
          .add(Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder setHeaders(final Map<String, List<String>> headers) {
      this.headers.clear();
      if (headers != null) {
        for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
          final List<String> values =
              entry.getValue() == null
                  ? Collections.emptyList()
                  : new ArrayList<>(entry.getValue());
          this.headers.put(entry.getKey(), values);
        }
      }
      return this;
    }

    public Builder setBody(final byte[] body) {
      this.body = body == null ? new byte[0] : body.clone();
      return this;
    }

    public Builder setTimeout(final Duration timeout) {
      if (timeout == null) {
        this.timeout = null;
        return this;
      }
      if (timeout.isNegative()) {
        throw new IllegalArgumentException("timeout must be non-negative");
      }
      this.timeout = timeout;
      return this;
    }

    /** Sets an inclusive buffered response-body byte limit for this request. */
    public Builder setMaximumResponseBytes(final Long maximumResponseBytes) {
      if (maximumResponseBytes != null) {
        BoundedResponseBodyReader.validateMaximum(maximumResponseBytes.longValue());
      }
      this.maximumResponseBytes = maximumResponseBytes;
      return this;
    }

    public TransportRequest build() {
      return new TransportRequest(
          method, uri, copyHeaders(headers), body, timeout, maximumResponseBytes);
    }

    private static Map<String, List<String>> copyHeaders(final Map<String, List<String>> source) {
      final Map<String, List<String>> copy = new java.util.LinkedHashMap<>();
      for (final Map.Entry<String, List<String>> entry : source.entrySet()) {
        final List<String> values =
            entry.getValue() == null
                ? Collections.emptyList()
                : Collections.unmodifiableList(new ArrayList<>(entry.getValue()));
        copy.put(entry.getKey(), values);
      }
      return copy;
    }
  }

  private static RequestReplayPolicy deriveReplayPolicy(
      final String method, final Map<String, List<String>> headers, final byte[] body) {
    final String normalizedMethod = method.toUpperCase(Locale.ROOT);
    final boolean readOnlyMethod =
        "GET".equals(normalizedMethod)
            || "HEAD".equals(normalizedMethod)
            || "OPTIONS".equals(normalizedMethod);
    boolean carriesOneShotHeader = false;
    for (final String name : headers.keySet()) {
      final String normalizedName = name.toLowerCase(Locale.ROOT);
      if ("x-iroha-signature".equals(normalizedName)
          || "x-iroha-nonce".equals(normalizedName)
          || "x-iroha-onboarding-token".equals(normalizedName)) {
        carriesOneShotHeader = true;
        break;
      }
    }
    return readOnlyMethod && body.length == 0 && !carriesOneShotHeader
        ? RequestReplayPolicy.RETRY_SAFE
        : RequestReplayPolicy.ONE_SHOT;
  }
}
