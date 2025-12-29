package org.hyperledger.iroha.android.client.transport;

import java.net.URI;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** SDK-owned transport request wrapper to decouple callers from {@code java.net.http.HttpRequest}. */
public final class TransportRequest {

  private final String method;
  private final URI uri;
  private final Map<String, List<String>> headers;
  private final byte[] body;
  private final Duration timeout;

  private TransportRequest(
      final String method,
      final URI uri,
      final Map<String, List<String>> headers,
      final byte[] body,
      final Duration timeout) {
    this.method = Objects.requireNonNull(method, "method");
    this.uri = Objects.requireNonNull(uri, "uri");
    this.headers = Collections.unmodifiableMap(headers);
    this.body = body == null ? new byte[0] : body.clone();
    this.timeout = timeout;
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

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {

    private String method = "GET";
    private URI uri = URI.create("http://localhost/");
    private final Map<String, List<String>> headers = new java.util.LinkedHashMap<>();
    private byte[] body = new byte[0];
    private Duration timeout = null;

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
              entry.getValue() == null ? List.of() : new ArrayList<>(entry.getValue());
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

    public TransportRequest build() {
      return new TransportRequest(method, uri, copyHeaders(headers), body, timeout);
    }

    private static Map<String, List<String>> copyHeaders(final Map<String, List<String>> source) {
      final Map<String, List<String>> copy = new java.util.LinkedHashMap<>();
      for (final Map.Entry<String, List<String>> entry : source.entrySet()) {
        final List<String> values =
            entry.getValue() == null ? List.of() : Collections.unmodifiableList(new ArrayList<>(entry.getValue()));
        copy.put(entry.getKey(), values);
      }
      return copy;
    }
  }
}
